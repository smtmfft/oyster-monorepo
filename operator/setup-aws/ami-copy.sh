#!/bin/bash

# Params : <profile> <family> <date> <source_region> <destination_region1> <destination_region2> <destination_region3>...

set -e

# set AWS_PROFILE 
export AWS_PROFILE=$1

# set AWS_DEFAULT_REGION 
export AWS_DEFAULT_REGION=$4

# List of regions the ami's will be copied to
regions=("${@:5}")

# Fetching ImageID for arm64 architecture
ami_arm=$(aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-arm64-$3" --no-paginate --query 'Images[0].ImageId')
ami_arm=$(echo $ami_arm| cut -d'"' -f 2)
echo "Source image id for arm64: $ami_arm"

# Fetching ImageID for amd64 architecture
ami_amd=$(aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-amd64-$3" --no-paginate --query 'Images[0].ImageId')
ami_amd=$(echo $ami_amd| cut -d'"' -f 2)
echo "Source image id for amd64: $ami_amd"

# Copying both AMI's to each of the secified regions
for r in ${regions[@]}; do
    old_ami_amd=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-amd64-$3" --no-paginate --query 'Images[0].ImageId')
    old_ami_arm=$(AWS_REGION=$r aws ec2 describe-images --owners self --filters "Name=name,Values=marlin/oyster/worker-$2-arm64-$3" --no-paginate --query 'Images[0].ImageId')
    if [[ $old_ami_amd = null ]]; then 
        echo "Copying amd64 for region: $r"
        aws ec2 copy-image --name "marlin/oyster/worker-$2-amd64-$3" --source-image-id $ami_amd --source-region $AWS_DEFAULT_REGION --region $r --copy-image-tags
    else
        echo "Found existing amd64 image in $r: $old_ami_amd"
    fi
    if [[ $old_ami_arm = null ]]; then 
        echo "Copying arm64 for region: $r"
        aws ec2 copy-image --name "marlin/oyster/worker-$2-arm64-$3" --source-image-id $ami_arm --source-region $AWS_DEFAULT_REGION --region $r --copy-image-tags
    else
        echo "Found existing arm64 image in $r: $old_ami_arm"
    fi
done
