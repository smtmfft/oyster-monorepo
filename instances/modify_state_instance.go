package instances

import (
	"fmt"
	// "encoding/json"
	// "io/ioutil"

	"github.com/aws/aws-sdk-go/aws"
	"github.com/aws/aws-sdk-go/aws/awserr"
	"github.com/aws/aws-sdk-go/aws/session"
	"github.com/aws/aws-sdk-go/service/ec2"
	log "github.com/sirupsen/logrus"
)

func CreateInstance(client *ec2.EC2, imageId string, minCount int, maxCount int, instanceType string, keyName string) (*ec2.Reservation, error) {
	res, err := client.RunInstances(&ec2.RunInstancesInput{
		ImageId:      aws.String(imageId),
		MinCount:     aws.Int64(int64(minCount)),
		MaxCount:     aws.Int64(int64(maxCount)),
		InstanceType: aws.String(instanceType),
		KeyName:      aws.String(keyName),
		EnclaveOptions: &ec2.EnclaveOptionsRequest{Enabled: &[]bool{true}[0]},
		BlockDeviceMappings: []*ec2.BlockDeviceMapping{&ec2.BlockDeviceMapping{
			Ebs: &[]ec2.EbsBlockDevice{ec2.EbsBlockDevice{VolumeSize: &[]int64{25}[0]}}[0],
			DeviceName: &[]string{"/dev/sda1"}[0],
		}},
	})

	if err != nil {
		return nil, err
	}
	_, errtag := client.CreateTags(&ec2.CreateTagsInput{
        Resources: []*string{res.Instances[0].InstanceId},
        Tags: []*ec2.Tag{
            {
                Key:   aws.String("Name"),
                Value: aws.String("TestRunner"),
            },
			{
				Key: aws.String("managedBy"),
				Value: aws.String("marlin"),
			},
        },
    })
	if errtag != nil {
        log.Warn("Could not create tags for instance", res.Instances[0].InstanceId, errtag)
    }
	return res, nil
}


func LaunchInstance(keyPairName string, profile string, region string) (*string) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session: %v", err)
		return nil
	}

	ec2Client := ec2.New(sess)

	keyName := keyPairName
	instanceType := "c6a.xlarge"
	minCount := 1
	maxCount := 1
	imageId := "ami-05ba3a39a75be1ec4"
	newInstance, err := CreateInstance(ec2Client, imageId, minCount, maxCount, instanceType, keyName)
	if err != nil {
		log.Error("Couldn't create new instance: %v", err)
		return nil
	}
	instanceID := newInstance.Instances[0].InstanceId

	log.Info("Instance Created: ")
	fmt.Printf("Created new instance: %v\n", newInstance.Instances)

	// uncomment to store details of newly created instance :
	// file, _ := json.MarshalIndent(newInstance.Instances[0], "", " ")
	// _ = ioutil.WriteFile("create_out.json", file, 0644)
	
	return instanceID
}

func RebootInstance(instanceID string, profile string, region string) {
	sess, err := session.NewSessionWithOptions(session.Options{
		Profile: profile,
		Config: aws.Config{
			Region: aws.String(region),
		},
	})

	if err != nil {
		log.Error("Failed to initialize new session: %v", err)
		return
	}

    // Create new EC2 client
    svc := ec2.New(sess)

    // We set DryRun to true to check to see if the instance exists and we have the
    // necessary permissions to monitor the instance.
    input := &ec2.RebootInstancesInput{
        InstanceIds: []*string{
            &instanceID,
        },
        DryRun: aws.Bool(true),
    }
    result, err := svc.RebootInstances(input)
    awsErr, ok := err.(awserr.Error)

    // If the error code is `DryRunOperation` it means we have the necessary
    // permissions to Start this instance
    if ok && awsErr.Code() == "DryRunOperation" {
        // Let's now set dry run to be false. This will allow us to reboot the instances
        input.DryRun = aws.Bool(false)
        result, err = svc.RebootInstances(input)
        if err != nil {
            log.Error("Error rebooting instance: ", err)
        } else {
            log.Info("Reboot Success!", result)
        }
    } else { // This could be due to a lack of permissions
        log.Warn("Error in reboot dry run: ", err)
    }
}