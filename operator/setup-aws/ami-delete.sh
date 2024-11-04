#!/bin/bash

# Params : <profile> <family> <date> <region1> <region2> <region3>...

set -e

# set AWS_PROFILE
export AWS_PROFILE=$1

# List of regions the amis will be deleted from
regions=("${@:4}")

# Copying both AMI's to each of the secified regions
for r in ${regions[@]}; do
    ami_amd=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-amd64-$3" --no-paginate --query 'Images[0].ImageId' --output text)
    snap_amd=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-amd64-$3" --no-paginate --query 'Images[0].BlockDeviceMappings[0].Ebs.SnapshotId' --output text)
    ami_arm=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-arm64-$3" --no-paginate --query 'Images[0].ImageId' --output text)
    snap_arm=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-arm64-$3" --no-paginate --query 'Images[0].BlockDeviceMappings[0].Ebs.SnapshotId' --output text)
    if [[ $ami_amd != null && "$ami_amd" != "None" ]]; then
        echo "Deleting amd64 image in region $r: $ami_amd"
        AWS_REGION=$r aws ec2 deregister-image --image-id $ami_amd
        AWS_REGION=$r aws ec2 delete-snapshot --snapshot-id $snap_amd
    else
        echo "Found no amd64 image in $r"
    fi
    if [[ $ami_arm != null && "$ami_arm" != "None" ]]; then
        echo "Deleting arm64 image in region $r: $ami_arm"
        AWS_REGION=$r aws ec2 deregister-image --image-id $ami_arm
        AWS_REGION=$r aws ec2 delete-snapshot --snapshot-id $snap_arm
    else
        echo "Found no arm64 image in $r"
    fi
done
